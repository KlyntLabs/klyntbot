//! Tests for FinanceFeature as FeaturePackage (AC 5.7).
//!
//! Verifies that FinanceFeature correctly implements the FeaturePackage trait:
//! - name(), config_key(), tools(), migrations(), default_config()
//! - Static methods: migrations_static(), default_config_static()

use feature_finance::{FeaturePackage, FinanceFeature};

fn make_feature() -> FinanceFeature {
    FinanceFeature::for_tests()
}

// ============================================================
// AC 5.7: name() returns "finance"
// ============================================================

#[test]
fn test_finance_feature_name() {
    assert_eq!(make_feature().name(), "finance");
}

// ============================================================
// AC 5.7: config_key() returns "finance"
// ============================================================

#[test]
fn test_finance_feature_config_key() {
    assert_eq!(make_feature().config_key(), "finance");
}

// ============================================================
// AC 5.7: tools() returns 1 tool named "finance"
// ============================================================

#[test]
fn test_finance_feature_tools_count() {
    assert_eq!(make_feature().tools().len(), 1);
}

#[test]
fn test_finance_feature_tools_name() {
    let feature = make_feature();
    let tools = feature.tools();
    assert_eq!(tools[0].name(), "finance");
}

// ============================================================
// AC 5.7: migrations() returns non-empty vec
// ============================================================

#[test]
fn test_finance_feature_migrations_not_empty() {
    let migrations = FinanceFeature::migrations_static();
    assert!(!migrations.is_empty(), "migrations_static() must return at least one migration");
}

#[test]
fn test_finance_feature_migrations_ordered_by_version() {
    let migrations = FinanceFeature::migrations_static();
    for window in migrations.windows(2) {
        assert!(
            window[0].version < window[1].version,
            "migrations must be in ascending version order: {} >= {}",
            window[0].version,
            window[1].version
        );
    }
}

#[test]
fn test_finance_feature_migrations_have_sql() {
    let migrations = FinanceFeature::migrations_static();
    for m in &migrations {
        assert!(
            !m.sql.is_empty(),
            "migration v{} has empty SQL content",
            m.version
        );
    }
}

// ============================================================
// AC 5.7: default_config() returns valid JSON
// ============================================================

#[test]
fn test_finance_feature_default_config_is_object() {
    let config = FinanceFeature::default_config_static();
    assert!(config.is_object(), "default_config_static() must return a JSON object");
}

#[test]
fn test_finance_feature_default_config_has_expected_keys() {
    let config = FinanceFeature::default_config_static();
    let obj = config.as_object().expect("config must be a JSON object");
    // FinanceConfig uses #[serde(rename_all = "camelCase")]:
    //   enabled → "enabled", default_currency → "defaultCurrency"
    let has_key = obj.contains_key("enabled") || obj.contains_key("defaultCurrency");
    assert!(
        has_key,
        "default config must contain 'enabled' or 'defaultCurrency'. \
         Found keys: {:?}",
        obj.keys().collect::<Vec<_>>()
    );
}

// ============================================================
// AC 5.7: migrations have feature_name "finance"
// ============================================================

#[test]
fn test_finance_feature_migrations_feature_name() {
    let migrations = FinanceFeature::migrations_static();
    for m in &migrations {
        assert_eq!(
            m.feature_name, "finance",
            "migration v{} has wrong feature_name '{}'",
            m.version, m.feature_name
        );
    }
}

#[test]
fn test_finance_feature_migrations_have_descriptions() {
    let migrations = FinanceFeature::migrations_static();
    for m in &migrations {
        assert!(
            !m.description.is_empty(),
            "migration v{} has empty description",
            m.version
        );
    }
}

// ============================================================
// AC 5.7: SQL migrations use IF NOT EXISTS
// ============================================================

#[test]
fn test_finance_feature_migrations_idempotent_ddl() {
    let migrations = FinanceFeature::migrations_static();
    for m in &migrations {
        if m.sql.contains("CREATE") {
            assert!(
                m.sql.contains("IF NOT EXISTS"),
                "migration v{} has a CREATE statement without IF NOT EXISTS",
                m.version
            );
        }
    }
}
