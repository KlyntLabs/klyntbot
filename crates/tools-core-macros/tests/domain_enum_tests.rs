//! Tests for `#[derive(DomainEnum)]` proc macro (AC 2.2).
//!
//! DomainEnum generates:
//! - `as_str()` returning snake_case canonical form
//! - `from_str_loose()` for case-insensitive parse with alias support
//! - `Display` delegating to `as_str()`
//! - `FromStr` delegating to `from_str_loose()`
//!
//! Dev: implement these tests via TDD alongside the macro in
//! `crates/tools-core-macros/src/domain_enum.rs`.

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
    // Verifies: as_str() converts PascalCase variant names to snake_case.
    // AC 2.2
    todo!()
}

#[test]
fn test_as_str_single_word_lowercase() {
    // Verifies: Single-word variants like `Todo` → "todo", `Archived` → "archived".
    // AC 2.2
    todo!()
}

// ============================================================
// AC 2.2: from_str_loose with canonical names
// ============================================================

#[test]
fn test_from_str_loose_canonical_names() {
    // Verifies: from_str_loose("todo") → Some(TestStatus::Todo), etc.
    // AC 2.2
    todo!()
}

// ============================================================
// AC 2.2: from_str_loose with aliases
// ============================================================

#[test]
fn test_from_str_loose_aliases() {
    // Verifies: from_str_loose("pending") → Some(TestStatus::Todo),
    //           from_str_loose("open") → Some(TestStatus::Todo),
    //           from_str_loose("in_progress") → Some(TestStatus::Doing),
    //           from_str_loose("active") → Some(TestStatus::Doing),
    //           from_str_loose("completed") → Some(TestStatus::Done),
    //           from_str_loose("closed") → Some(TestStatus::Done).
    // AC 2.2
    todo!()
}

// ============================================================
// AC 2.2: from_str_loose case insensitive
// ============================================================

#[test]
fn test_from_str_loose_case_insensitive() {
    // Verifies: from_str_loose("TODO") → Some(TestStatus::Todo),
    //           from_str_loose("Doing") → Some(TestStatus::Doing),
    //           from_str_loose("ARCHIVED") → Some(TestStatus::Archived),
    //           from_str_loose("Pending") → Some(TestStatus::Todo).
    // AC 2.2
    todo!()
}

// ============================================================
// AC 2.2: from_str_loose unknown returns None
// ============================================================

#[test]
fn test_from_str_loose_unknown_returns_none() {
    // Verifies: from_str_loose("unknown") → None,
    //           from_str_loose("") → None,
    //           from_str_loose("  ") → None.
    // AC 2.2
    todo!()
}

// ============================================================
// AC 2.2: Display delegates to as_str
// ============================================================

#[test]
fn test_display_delegates_to_as_str() {
    // Verifies: format!("{}", TestStatus::Todo) == "todo",
    //           format!("{}", TestStatus::Doing) == "doing",
    //           format!("{}", TestStatus::Done) == "done".
    // AC 2.2
    todo!()
}

// ============================================================
// AC 2.2: FromStr delegates to from_str_loose
// ============================================================

#[test]
fn test_from_str_trait_success() {
    // Verifies: "todo".parse::<TestStatus>() == Ok(TestStatus::Todo),
    //           "pending".parse::<TestStatus>() == Ok(TestStatus::Todo).
    // AC 2.2
    todo!()
}

#[test]
fn test_from_str_trait_error() {
    // Verifies: "unknown".parse::<TestStatus>().is_err().
    // AC 2.2
    todo!()
}

// ============================================================
// AC 2.2: CamelCase → snake_case conversion
// ============================================================

#[test]
fn test_camel_case_to_snake_case_in_progress() {
    // Verifies: MultiWordVariant::InProgress.as_str() == "in_progress".
    // AC 2.2
    todo!()
}

#[test]
fn test_camel_case_to_snake_case_crypto_wallet() {
    // Verifies: MultiWordVariant::CryptoWallet.as_str() == "crypto_wallet".
    // AC 2.2
    todo!()
}

#[test]
fn test_camel_case_to_snake_case_six_jar() {
    // Verifies: MultiWordVariant::SixJar.as_str() == "six_jar".
    // AC 2.2
    todo!()
}

#[test]
fn test_camel_case_to_snake_case_debt_payoff() {
    // Verifies: MultiWordVariant::DebtPayoff.as_str() == "debt_payoff".
    // AC 2.2
    todo!()
}

#[test]
fn test_camel_case_to_snake_case_real_estate() {
    // Verifies: MultiWordVariant::RealEstate.as_str() == "real_estate".
    // AC 2.2
    todo!()
}

// ============================================================
// AC 2.2: #[canonical] override
// ============================================================

#[test]
fn test_canonical_override() {
    // Verifies: CanonicalOverride::Ewallet.as_str() == "e_wallet" (not "ewallet").
    // AC 2.2
    todo!()
}

#[test]
fn test_canonical_override_from_str() {
    // Verifies: CanonicalOverride::from_str_loose("e_wallet") == Some(CanonicalOverride::Ewallet).
    // AC 2.2
    todo!()
}
