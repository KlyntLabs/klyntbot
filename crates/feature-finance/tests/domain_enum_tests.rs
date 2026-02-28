//! Parametric tests for Finance domain enums (DomainEnum derive).
//!
//! Each of the 10 finance enums gets exactly 2 tests:
//!   - `as_str_returns_expected_for_all_variants` — round-trips every variant
//!   - `from_str_loose_resolves_aliases_and_case` — canonical names, aliases,
//!     case variations, and the `None` (invalid) case
//!
//! Cross-cutting tests verify that Display delegates to `as_str` and FromStr
//! delegates to `from_str_loose` for all enums.

// ============================================================
// 1. AccountType
// ============================================================

mod account_type {
    use feature_finance::types::AccountType;

    #[test]
    fn as_str_returns_expected_for_all_variants() {
        let cases = [
            (AccountType::Cash, "cash"),
            (AccountType::Bank, "bank"),
            (AccountType::Ewallet, "ewallet"),
            (AccountType::CryptoWallet, "crypto_wallet"),
            (AccountType::Brokerage, "brokerage"),
        ];
        for (variant, expected) in cases {
            assert_eq!(
                variant.as_str(),
                expected,
                "as_str failed for {:?}",
                variant
            );
        }
    }

    #[test]
    fn from_str_loose_resolves_aliases_and_case() {
        let cases: &[(&str, Option<AccountType>)] = &[
            ("cash", Some(AccountType::Cash)),
            ("CASH", Some(AccountType::Cash)),
            ("bank", Some(AccountType::Bank)),
            ("Bank", Some(AccountType::Bank)),
            ("ewallet", Some(AccountType::Ewallet)),
            ("e_wallet", Some(AccountType::Ewallet)),
            ("crypto_wallet", Some(AccountType::CryptoWallet)),
            ("cryptowallet", Some(AccountType::CryptoWallet)),
            ("brokerage", Some(AccountType::Brokerage)),
            ("invalid", None),
        ];
        for (input, expected) in cases {
            assert_eq!(
                AccountType::from_str_loose(input),
                *expected,
                "from_str_loose({input}) failed"
            );
        }
    }
}

// ============================================================
// 2. TransactionType
// ============================================================

mod transaction_type {
    use feature_finance::types::TransactionType;

    #[test]
    fn as_str_returns_expected_for_all_variants() {
        let cases = [
            (TransactionType::Income, "income"),
            (TransactionType::Expense, "expense"),
            (TransactionType::Transfer, "transfer"),
        ];
        for (variant, expected) in cases {
            assert_eq!(
                variant.as_str(),
                expected,
                "as_str failed for {:?}",
                variant
            );
        }
    }

    #[test]
    fn from_str_loose_resolves_aliases_and_case() {
        let cases: &[(&str, Option<TransactionType>)] = &[
            ("income", Some(TransactionType::Income)),
            ("INCOME", Some(TransactionType::Income)),
            ("expense", Some(TransactionType::Expense)),
            ("transfer", Some(TransactionType::Transfer)),
            ("invalid", None),
        ];
        for (input, expected) in cases {
            assert_eq!(
                TransactionType::from_str_loose(input),
                *expected,
                "from_str_loose({input}) failed"
            );
        }
    }
}

// ============================================================
// 3. BudgetPeriod
// ============================================================

mod budget_period {
    use feature_finance::types::BudgetPeriod;

    #[test]
    fn as_str_returns_expected_for_all_variants() {
        let cases = [
            (BudgetPeriod::Monthly, "monthly"),
            (BudgetPeriod::Weekly, "weekly"),
            (BudgetPeriod::Yearly, "yearly"),
            (BudgetPeriod::Custom, "custom"),
        ];
        for (variant, expected) in cases {
            assert_eq!(
                variant.as_str(),
                expected,
                "as_str failed for {:?}",
                variant
            );
        }
    }

    #[test]
    fn from_str_loose_resolves_aliases_and_case() {
        let cases: &[(&str, Option<BudgetPeriod>)] = &[
            ("monthly", Some(BudgetPeriod::Monthly)),
            ("month", Some(BudgetPeriod::Monthly)),
            ("weekly", Some(BudgetPeriod::Weekly)),
            ("week", Some(BudgetPeriod::Weekly)),
            ("yearly", Some(BudgetPeriod::Yearly)),
            ("year", Some(BudgetPeriod::Yearly)),
            ("annual", Some(BudgetPeriod::Yearly)),
            ("custom", Some(BudgetPeriod::Custom)),
            ("invalid", None),
        ];
        for (input, expected) in cases {
            assert_eq!(
                BudgetPeriod::from_str_loose(input),
                *expected,
                "from_str_loose({input}) failed"
            );
        }
    }
}

// ============================================================
// 4. BudgetMethod
// ============================================================

mod budget_method {
    use feature_finance::types::BudgetMethod;

    #[test]
    fn as_str_returns_expected_for_all_variants() {
        let cases = [
            (BudgetMethod::Standard, "standard"),
            (BudgetMethod::SixJar, "six_jar"),
        ];
        for (variant, expected) in cases {
            assert_eq!(
                variant.as_str(),
                expected,
                "as_str failed for {:?}",
                variant
            );
        }
    }

    #[test]
    fn from_str_loose_resolves_aliases_and_case() {
        let cases: &[(&str, Option<BudgetMethod>)] = &[
            ("standard", Some(BudgetMethod::Standard)),
            ("six_jar", Some(BudgetMethod::SixJar)),
            ("sixjar", Some(BudgetMethod::SixJar)),
            ("6jar", Some(BudgetMethod::SixJar)),
            ("invalid", None),
        ];
        for (input, expected) in cases {
            assert_eq!(
                BudgetMethod::from_str_loose(input),
                *expected,
                "from_str_loose({input}) failed"
            );
        }
    }
}

// ============================================================
// 5. JarType
// ============================================================

mod jar_type {
    use feature_finance::types::JarType;

    #[test]
    fn as_str_returns_expected_for_all_variants() {
        let cases = [
            (JarType::Essentials, "essentials"),
            (JarType::Savings, "savings"),
            (JarType::Investment, "investment"),
            (JarType::Education, "education"),
            (JarType::Entertainment, "entertainment"),
            (JarType::Charity, "charity"),
        ];
        for (variant, expected) in cases {
            assert_eq!(
                variant.as_str(),
                expected,
                "as_str failed for {:?}",
                variant
            );
        }
    }

    #[test]
    fn from_str_loose_resolves_aliases_and_case() {
        let cases: &[(&str, Option<JarType>)] = &[
            ("essentials", Some(JarType::Essentials)),
            ("necessities", Some(JarType::Essentials)),
            ("savings", Some(JarType::Savings)),
            ("saving", Some(JarType::Savings)),
            ("investment", Some(JarType::Investment)),
            ("investments", Some(JarType::Investment)),
            ("education", Some(JarType::Education)),
            ("entertainment", Some(JarType::Entertainment)),
            ("play", Some(JarType::Entertainment)),
            ("charity", Some(JarType::Charity)),
            ("give", Some(JarType::Charity)),
            ("invalid", None),
        ];
        for (input, expected) in cases {
            assert_eq!(
                JarType::from_str_loose(input),
                *expected,
                "from_str_loose({input}) failed"
            );
        }
    }
}

// ============================================================
// 6. AssetType
// ============================================================

mod asset_type {
    use feature_finance::types::AssetType;

    #[test]
    fn as_str_returns_expected_for_all_variants() {
        let cases = [
            (AssetType::Stock, "stock"),
            (AssetType::Etf, "etf"),
            (AssetType::Crypto, "crypto"),
            (AssetType::RealEstate, "real_estate"),
            (AssetType::Bond, "bond"),
            (AssetType::ExchangeRate, "exchange_rate"),
        ];
        for (variant, expected) in cases {
            assert_eq!(
                variant.as_str(),
                expected,
                "as_str failed for {:?}",
                variant
            );
        }
    }

    #[test]
    fn from_str_loose_resolves_aliases_and_case() {
        let cases: &[(&str, Option<AssetType>)] = &[
            ("stock", Some(AssetType::Stock)),
            ("stocks", Some(AssetType::Stock)),
            ("equity", Some(AssetType::Stock)),
            ("etf", Some(AssetType::Etf)),
            ("crypto", Some(AssetType::Crypto)),
            ("cryptocurrency", Some(AssetType::Crypto)),
            ("real_estate", Some(AssetType::RealEstate)),
            ("property", Some(AssetType::RealEstate)),
            ("bond", Some(AssetType::Bond)),
            ("bonds", Some(AssetType::Bond)),
            ("fixed_income", Some(AssetType::Bond)),
            ("exchange_rate", Some(AssetType::ExchangeRate)),
            ("forex", Some(AssetType::ExchangeRate)),
            ("fx", Some(AssetType::ExchangeRate)),
            ("invalid", None),
        ];
        for (input, expected) in cases {
            assert_eq!(
                AssetType::from_str_loose(input),
                *expected,
                "from_str_loose({input}) failed"
            );
        }
    }
}

// ============================================================
// 7. InvestmentTxType
// ============================================================

mod investment_tx_type {
    use feature_finance::types::InvestmentTxType;

    #[test]
    fn as_str_returns_expected_for_all_variants() {
        let cases = [
            (InvestmentTxType::Buy, "buy"),
            (InvestmentTxType::Sell, "sell"),
            (InvestmentTxType::Dividend, "dividend"),
            (InvestmentTxType::RentalIncome, "rental_income"),
            (InvestmentTxType::Interest, "interest"),
            (InvestmentTxType::Split, "split"),
        ];
        for (variant, expected) in cases {
            assert_eq!(
                variant.as_str(),
                expected,
                "as_str failed for {:?}",
                variant
            );
        }
    }

    #[test]
    fn from_str_loose_resolves_aliases_and_case() {
        let cases: &[(&str, Option<InvestmentTxType>)] = &[
            ("buy", Some(InvestmentTxType::Buy)),
            ("purchase", Some(InvestmentTxType::Buy)),
            ("sell", Some(InvestmentTxType::Sell)),
            ("sale", Some(InvestmentTxType::Sell)),
            ("dividend", Some(InvestmentTxType::Dividend)),
            ("rental_income", Some(InvestmentTxType::RentalIncome)),
            ("rental", Some(InvestmentTxType::RentalIncome)),
            ("rent", Some(InvestmentTxType::RentalIncome)),
            ("interest", Some(InvestmentTxType::Interest)),
            ("split", Some(InvestmentTxType::Split)),
            ("invalid", None),
        ];
        for (input, expected) in cases {
            assert_eq!(
                InvestmentTxType::from_str_loose(input),
                *expected,
                "from_str_loose({input}) failed"
            );
        }
    }
}

// ============================================================
// 8. GoalType
// ============================================================

mod goal_type {
    use feature_finance::types::GoalType;

    #[test]
    fn as_str_returns_expected_for_all_variants() {
        let cases = [
            (GoalType::Savings, "savings"),
            (GoalType::Purchase, "purchase"),
            (GoalType::DebtPayoff, "debt_payoff"),
            (GoalType::Fire, "fire"),
        ];
        for (variant, expected) in cases {
            assert_eq!(
                variant.as_str(),
                expected,
                "as_str failed for {:?}",
                variant
            );
        }
    }

    #[test]
    fn from_str_loose_resolves_aliases_and_case() {
        let cases: &[(&str, Option<GoalType>)] = &[
            ("savings", Some(GoalType::Savings)),
            ("saving", Some(GoalType::Savings)),
            ("emergency_fund", Some(GoalType::Savings)),
            ("purchase", Some(GoalType::Purchase)),
            ("buy", Some(GoalType::Purchase)),
            ("debt_payoff", Some(GoalType::DebtPayoff)),
            ("debt", Some(GoalType::DebtPayoff)),
            ("payoff", Some(GoalType::DebtPayoff)),
            ("fire", Some(GoalType::Fire)),
            ("financial_independence", Some(GoalType::Fire)),
            ("invalid", None),
        ];
        for (input, expected) in cases {
            assert_eq!(
                GoalType::from_str_loose(input),
                *expected,
                "from_str_loose({input}) failed"
            );
        }
    }
}

// ============================================================
// 9. GoalStatus
// ============================================================

mod goal_status {
    use feature_finance::types::GoalStatus;

    #[test]
    fn as_str_returns_expected_for_all_variants() {
        let cases = [
            (GoalStatus::Active, "active"),
            (GoalStatus::Achieved, "achieved"),
            (GoalStatus::Abandoned, "abandoned"),
        ];
        for (variant, expected) in cases {
            assert_eq!(
                variant.as_str(),
                expected,
                "as_str failed for {:?}",
                variant
            );
        }
    }

    #[test]
    fn from_str_loose_resolves_aliases_and_case() {
        let cases: &[(&str, Option<GoalStatus>)] = &[
            ("active", Some(GoalStatus::Active)),
            ("in_progress", Some(GoalStatus::Active)),
            ("achieved", Some(GoalStatus::Achieved)),
            ("completed", Some(GoalStatus::Achieved)),
            ("done", Some(GoalStatus::Achieved)),
            ("abandoned", Some(GoalStatus::Abandoned)),
            ("cancelled", Some(GoalStatus::Abandoned)),
            ("invalid", None),
        ];
        for (input, expected) in cases {
            assert_eq!(
                GoalStatus::from_str_loose(input),
                *expected,
                "from_str_loose({input}) failed"
            );
        }
    }
}

// ============================================================
// 10. LiabilityType
// ============================================================

mod liability_type {
    use feature_finance::types::LiabilityType;

    #[test]
    fn as_str_returns_expected_for_all_variants() {
        let cases = [
            (LiabilityType::Mortgage, "mortgage"),
            (LiabilityType::CreditCard, "credit_card"),
            (LiabilityType::PersonalLoan, "personal_loan"),
            (LiabilityType::StudentLoan, "student_loan"),
            (LiabilityType::Other, "other"),
        ];
        for (variant, expected) in cases {
            assert_eq!(
                variant.as_str(),
                expected,
                "as_str failed for {:?}",
                variant
            );
        }
    }

    #[test]
    fn from_str_loose_resolves_aliases_and_case() {
        let cases: &[(&str, Option<LiabilityType>)] = &[
            ("mortgage", Some(LiabilityType::Mortgage)),
            ("MORTGAGE", Some(LiabilityType::Mortgage)),
            ("home_loan", Some(LiabilityType::Mortgage)),
            ("credit_card", Some(LiabilityType::CreditCard)),
            ("creditcard", Some(LiabilityType::CreditCard)),
            ("cc", Some(LiabilityType::CreditCard)),
            ("personal_loan", Some(LiabilityType::PersonalLoan)),
            ("personal", Some(LiabilityType::PersonalLoan)),
            ("student_loan", Some(LiabilityType::StudentLoan)),
            ("student", Some(LiabilityType::StudentLoan)),
            ("education_loan", Some(LiabilityType::StudentLoan)),
            ("other", Some(LiabilityType::Other)),
            ("invalid", None),
        ];
        for (input, expected) in cases {
            assert_eq!(
                LiabilityType::from_str_loose(input),
                *expected,
                "from_str_loose({input}) failed"
            );
        }
    }
}

// ============================================================
// Cross-cutting: Display trait delegates to as_str for all enums
// ============================================================

mod display_trait {
    use feature_finance::types::{
        AccountType, AssetType, BudgetMethod, BudgetPeriod, GoalStatus, GoalType, InvestmentTxType,
        JarType, LiabilityType, TransactionType,
    };

    #[test]
    fn all_enums_display_matches_as_str() {
        assert_eq!(format!("{}", AccountType::Cash), AccountType::Cash.as_str());
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
        assert_eq!(format!("{}", AssetType::Stock), AssetType::Stock.as_str());
        assert_eq!(
            format!("{}", InvestmentTxType::Buy),
            InvestmentTxType::Buy.as_str()
        );
        assert_eq!(format!("{}", GoalType::Savings), GoalType::Savings.as_str());
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
        AccountType, AssetType, BudgetMethod, BudgetPeriod, GoalStatus, GoalType, InvestmentTxType,
        JarType, LiabilityType, TransactionType,
    };
    use std::str::FromStr;

    #[test]
    fn all_enums_from_str_delegates_to_from_str_loose() {
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
    fn all_enums_from_str_unknown_returns_err() {
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
