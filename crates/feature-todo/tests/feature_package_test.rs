//! Tests for TodoFeature as FeaturePackage (AC 4.9).
//!
//! Verifies that TodoFeature correctly implements the FeaturePackage trait:
//! - name(), config_key(), tools(), migrations(), default_config()
//! - Static methods: migrations_static(), default_config_static()
//!
//! Dev: implement these tests via TDD alongside the TodoFeature in
//! `crates/feature-todo/src/lib.rs`.

// NOTE: Adjust imports once feature-todo/src/lib.rs is finalized.
//
// Expected imports:
//   use feature_todo::TodoFeature;
//   use tools_core::FeaturePackage;
//   use serde_json::Value;

// ============================================================
// AC 4.9: name() returns "todo"
// ============================================================

#[test]
fn test_todo_feature_name() {
    // Verifies: TodoFeature.name() == "todo".
    //
    // Note: This requires constructing a TodoFeature, which needs a PgPool.
    // For unit tests without DB, use the static methods or test with a mock pool.
    //
    // Alternative: If TodoFeature::name() is trivially "todo", test via static method
    // or construct with test pool.
    // AC 4.9
    todo!()
}

// ============================================================
// AC 4.9: config_key() returns "todo"
// ============================================================

#[test]
fn test_todo_feature_config_key() {
    // Verifies: TodoFeature.config_key() == "todo".
    // AC 4.9
    todo!()
}

// ============================================================
// AC 4.9: tools() returns 1 tool named "todo"
// ============================================================

#[test]
fn test_todo_feature_tools_count() {
    // Verifies: TodoFeature.tools().len() == 1.
    // AC 4.9
    todo!()
}

#[test]
fn test_todo_feature_tools_name() {
    // Verifies: TodoFeature.tools()[0].name() == "todo".
    // AC 4.9
    todo!()
}

// ============================================================
// AC 4.9: migrations() returns non-empty vec
// ============================================================

#[test]
fn test_todo_feature_migrations_not_empty() {
    // Verifies: TodoFeature::migrations_static() is non-empty.
    // AC 4.9
    todo!()
}

#[test]
fn test_todo_feature_migrations_ordered_by_version() {
    // Verifies: migrations are in ascending version order (1, 2, 3, ...).
    // AC 4.9
    todo!()
}

#[test]
fn test_todo_feature_migrations_have_sql() {
    // Verifies: Each migration has non-empty SQL content.
    // AC 4.9
    todo!()
}

// ============================================================
// AC 4.9: default_config() returns valid JSON
// ============================================================

#[test]
fn test_todo_feature_default_config_is_object() {
    // Verifies: TodoFeature::default_config_static() is a JSON object.
    // AC 4.9
    todo!()
}

#[test]
fn test_todo_feature_default_config_has_expected_keys() {
    // Verifies: Default config contains expected keys like "focus", "search", "enrichment".
    // AC 4.9
    todo!()
}

// ============================================================
// AC 4.9: migrations_static() has feature_name "todo"
// ============================================================

#[test]
fn test_todo_feature_migrations_feature_name() {
    // Verifies: All migrations from migrations_static() have feature_name == "todo".
    // AC 4.9
    todo!()
}

#[test]
fn test_todo_feature_migrations_have_descriptions() {
    // Verifies: Each migration has a non-empty description.
    // AC 4.9
    todo!()
}

// ============================================================
// AC 4.9: SQL migrations use IF NOT EXISTS
// ============================================================

#[test]
fn test_todo_feature_migrations_idempotent_ddl() {
    // Verifies: Each migration's SQL contains "IF NOT EXISTS" for CREATE statements.
    // This ensures idempotency for pre-seeded databases.
    // AC 4.9
    todo!()
}
