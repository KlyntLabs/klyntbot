//! Tests for `#[derive(DomainEnum)]` proc macro (AC 2.2).

use tools_core_macros::DomainEnum;

// --- Test enum: basic variants with aliases ---

#[derive(Debug, Clone, PartialEq, Eq, DomainEnum)]
pub enum TestStatus {
    #[aliases("pending", "open")]
    Todo,
    #[aliases("in_progress", "active")]
    Doing,
    #[aliases("completed", "closed")]
    Done,
    Archived,
}

// --- Test enum: CamelCase → snake_case conversion ---

#[derive(Debug, Clone, PartialEq, Eq, DomainEnum)]
pub enum MultiWordVariant {
    InProgress,
    CryptoWallet,
    SixJar,
    DebtPayoff,
    RealEstate,
}

// --- Test enum: with #[canonical] override ---

#[derive(Debug, Clone, PartialEq, Eq, DomainEnum)]
pub enum CanonicalOverride {
    #[canonical("e_wallet")]
    Ewallet,
    Normal,
}

// ============================================================
// AC 2.2: as_str returns snake_case
// ============================================================

#[test]
fn test_as_str_returns_snake_case() {
    assert_eq!(MultiWordVariant::InProgress.as_str(), "in_progress");
    assert_eq!(MultiWordVariant::CryptoWallet.as_str(), "crypto_wallet");
}

#[test]
fn test_as_str_single_word_lowercase() {
    assert_eq!(TestStatus::Todo.as_str(), "todo");
    assert_eq!(TestStatus::Archived.as_str(), "archived");
}

// ============================================================
// AC 2.2: from_str_loose with canonical names
// ============================================================

#[test]
fn test_from_str_loose_canonical_names() {
    assert_eq!(TestStatus::from_str_loose("todo"), Some(TestStatus::Todo));
    assert_eq!(TestStatus::from_str_loose("doing"), Some(TestStatus::Doing));
    assert_eq!(TestStatus::from_str_loose("done"), Some(TestStatus::Done));
    assert_eq!(
        TestStatus::from_str_loose("archived"),
        Some(TestStatus::Archived)
    );
}

// ============================================================
// AC 2.2: from_str_loose with aliases
// ============================================================

#[test]
fn test_from_str_loose_aliases() {
    assert_eq!(
        TestStatus::from_str_loose("pending"),
        Some(TestStatus::Todo)
    );
    assert_eq!(TestStatus::from_str_loose("open"), Some(TestStatus::Todo));
    assert_eq!(
        TestStatus::from_str_loose("in_progress"),
        Some(TestStatus::Doing)
    );
    assert_eq!(
        TestStatus::from_str_loose("active"),
        Some(TestStatus::Doing)
    );
    assert_eq!(
        TestStatus::from_str_loose("completed"),
        Some(TestStatus::Done)
    );
    assert_eq!(TestStatus::from_str_loose("closed"), Some(TestStatus::Done));
}

// ============================================================
// AC 2.2: from_str_loose case insensitive
// ============================================================

#[test]
fn test_from_str_loose_case_insensitive() {
    assert_eq!(TestStatus::from_str_loose("TODO"), Some(TestStatus::Todo));
    assert_eq!(TestStatus::from_str_loose("Doing"), Some(TestStatus::Doing));
    assert_eq!(
        TestStatus::from_str_loose("ARCHIVED"),
        Some(TestStatus::Archived)
    );
    assert_eq!(
        TestStatus::from_str_loose("Pending"),
        Some(TestStatus::Todo)
    );
}

// ============================================================
// AC 2.2: from_str_loose unknown returns None
// ============================================================

#[test]
fn test_from_str_loose_unknown_returns_none() {
    assert_eq!(TestStatus::from_str_loose("unknown"), None);
    assert_eq!(TestStatus::from_str_loose(""), None);
    assert_eq!(TestStatus::from_str_loose("  "), None);
}

// ============================================================
// AC 2.2: Display delegates to as_str
// ============================================================

#[test]
fn test_display_delegates_to_as_str() {
    assert_eq!(format!("{}", TestStatus::Todo), "todo");
    assert_eq!(format!("{}", TestStatus::Doing), "doing");
    assert_eq!(format!("{}", TestStatus::Done), "done");
}

// ============================================================
// AC 2.2: FromStr delegates to from_str_loose
// ============================================================

#[test]
fn test_from_str_trait_success() {
    assert_eq!("todo".parse::<TestStatus>(), Ok(TestStatus::Todo));
    assert_eq!("pending".parse::<TestStatus>(), Ok(TestStatus::Todo));
}

#[test]
fn test_from_str_trait_error() {
    assert!("unknown".parse::<TestStatus>().is_err());
}

// ============================================================
// AC 2.2: CamelCase → snake_case conversion
// ============================================================

#[test]
fn test_camel_case_to_snake_case_in_progress() {
    assert_eq!(MultiWordVariant::InProgress.as_str(), "in_progress");
}

#[test]
fn test_camel_case_to_snake_case_crypto_wallet() {
    assert_eq!(MultiWordVariant::CryptoWallet.as_str(), "crypto_wallet");
}

#[test]
fn test_camel_case_to_snake_case_six_jar() {
    assert_eq!(MultiWordVariant::SixJar.as_str(), "six_jar");
}

#[test]
fn test_camel_case_to_snake_case_debt_payoff() {
    assert_eq!(MultiWordVariant::DebtPayoff.as_str(), "debt_payoff");
}

#[test]
fn test_camel_case_to_snake_case_real_estate() {
    assert_eq!(MultiWordVariant::RealEstate.as_str(), "real_estate");
}

// ============================================================
// AC 2.2: #[canonical] override
// ============================================================

#[test]
fn test_canonical_override() {
    assert_eq!(CanonicalOverride::Ewallet.as_str(), "e_wallet");
}

#[test]
fn test_canonical_override_from_str() {
    assert_eq!(
        CanonicalOverride::from_str_loose("e_wallet"),
        Some(CanonicalOverride::Ewallet)
    );
}
