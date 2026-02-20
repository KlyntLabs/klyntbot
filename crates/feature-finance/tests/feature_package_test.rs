//! Tests for FinanceFeature as FeaturePackage (AC 5.7).
//!
//! Verifies that FinanceFeature correctly implements the FeaturePackage trait:
//! - name(), config_key(), tools(), migrations(), default_config()
//! - Static methods: migrations_static(), default_config_static()
//!
//! Dev: implement these tests via TDD alongside the FinanceFeature in
//! `crates/feature-finance/src/lib.rs`.

// NOTE: Adjust imports once feature-finance/src/lib.rs is finalized.
//
// Expected imports:
//   use feature_finance::FinanceFeature;
//   use tools_core::FeaturePackage;
//   use serde_json::Value;

// ============================================================
// AC 5.7: name() returns "finance"
// ============================================================

#[test]
fn test_finance_feature_name() {
    // Verifies: FinanceFeature.name() == "finance".
    // AC 5.7
    todo!()
}

// ============================================================
// AC 5.7: config_key() returns "finance"
// ============================================================

#[test]
fn test_finance_feature_config_key() {
    // Verifies: FinanceFeature.config_key() == "finance".
    // AC 5.7
    todo!()
}

// ============================================================
// AC 5.7: tools() returns 1 tool named "finance"
// ============================================================

#[test]
fn test_finance_feature_tools_count() {
    // Verifies: FinanceFeature.tools().len() == 1.
    // AC 5.7
    todo!()
}

#[test]
fn test_finance_feature_tools_name() {
    // Verifies: FinanceFeature.tools()[0].name() == "finance".
    // AC 5.7
    todo!()
}

// ============================================================
// AC 5.7: migrations() returns non-empty vec
// ============================================================

#[test]
fn test_finance_feature_migrations_not_empty() {
    // Verifies: FinanceFeature::migrations_static() is non-empty.
    // AC 5.7
    todo!()
}

#[test]
fn test_finance_feature_migrations_ordered_by_version() {
    // Verifies: migrations are in ascending version order (1, 2, 3, ...).
    // AC 5.7
    todo!()
}

#[test]
fn test_finance_feature_migrations_have_sql() {
    // Verifies: Each migration has non-empty SQL content.
    // AC 5.7
    todo!()
}

// ============================================================
// AC 5.7: default_config() returns valid JSON
// ============================================================

#[test]
fn test_finance_feature_default_config_is_object() {
    // Verifies: FinanceFeature::default_config_static() is a JSON object.
    // AC 5.7
    todo!()
}

#[test]
fn test_finance_feature_default_config_has_expected_keys() {
    // Verifies: Default config contains expected keys like "enabled", "currency".
    // AC 5.7
    todo!()
}

// ============================================================
// AC 5.7: migrations have feature_name "finance"
// ============================================================

#[test]
fn test_finance_feature_migrations_feature_name() {
    // Verifies: All migrations from migrations_static() have feature_name == "finance".
    // AC 5.7
    todo!()
}

#[test]
fn test_finance_feature_migrations_have_descriptions() {
    // Verifies: Each migration has a non-empty description.
    // AC 5.7
    todo!()
}

// ============================================================
// AC 5.7: SQL migrations use IF NOT EXISTS
// ============================================================

#[test]
fn test_finance_feature_migrations_idempotent_ddl() {
    // Verifies: Each migration's SQL contains "IF NOT EXISTS" for CREATE statements.
    // AC 5.7
    todo!()
}
