//! Tests for `#[derive(ActionParams)]` proc macro (AC 2.3).

use serde_json::json;
use tools_core_macros::ActionParams;

// --- Test struct: mix of required, optional, constrained, and vec fields ---

#[derive(Debug, ActionParams)]
pub struct AddParams {
    /// Task title
    #[param(required)]
    pub title: String,

    /// Task priority (1-5)
    #[param(min = 1, max = 5)]
    pub priority: Option<u8>,

    /// Tags for categorization
    pub tags: Vec<String>,

    /// Optional description
    pub description: Option<String>,

    /// Whether the task is urgent
    pub urgent: Option<bool>,
}

// --- Test struct: empty (zero fields) ---

#[derive(Debug, ActionParams)]
pub struct EmptyParams {}

// --- Test struct: string length constraints ---

#[derive(Debug, ActionParams)]
pub struct ConstrainedStringParams {
    /// Short code
    #[param(required, min_length = 2, max_length = 10)]
    pub code: String,
}

// ============================================================
// AC 2.3: json_schema has required fields
// ============================================================

#[test]
fn test_json_schema_has_required_fields() {
    let schema = AddParams::json_schema();
    let required = schema["required"].as_array().unwrap();
    assert!(required.iter().any(|v| v == "title"));
}

#[test]
fn test_json_schema_required_does_not_include_optional() {
    let schema = AddParams::json_schema();
    let required = schema["required"].as_array().unwrap();
    assert!(!required.iter().any(|v| v == "priority"));
    assert!(!required.iter().any(|v| v == "tags"));
    assert!(!required.iter().any(|v| v == "description"));
    assert!(!required.iter().any(|v| v == "urgent"));
}

// ============================================================
// AC 2.3: json_schema has correct property types
// ============================================================

#[test]
fn test_json_schema_string_type() {
    let schema = AddParams::json_schema();
    assert_eq!(schema["properties"]["title"]["type"], "string");
}

#[test]
fn test_json_schema_integer_type() {
    let schema = AddParams::json_schema();
    assert_eq!(schema["properties"]["priority"]["type"], "integer");
}

#[test]
fn test_json_schema_boolean_type() {
    let schema = AddParams::json_schema();
    assert_eq!(schema["properties"]["urgent"]["type"], "boolean");
}

#[test]
fn test_json_schema_array_type() {
    let schema = AddParams::json_schema();
    assert_eq!(schema["properties"]["tags"]["type"], "array");
    assert_eq!(schema["properties"]["tags"]["items"]["type"], "string");
}

#[test]
fn test_json_schema_optional_string_type() {
    let schema = AddParams::json_schema();
    assert_eq!(schema["properties"]["description"]["type"], "string");
}

// ============================================================
// AC 2.3: json_schema includes min/max constraints
// ============================================================

#[test]
fn test_json_schema_min_max_constraints() {
    let schema = AddParams::json_schema();
    assert_eq!(schema["properties"]["priority"]["minimum"], 1);
    assert_eq!(schema["properties"]["priority"]["maximum"], 5);
}

#[test]
fn test_json_schema_min_max_length_constraints() {
    let schema = ConstrainedStringParams::json_schema();
    assert_eq!(schema["properties"]["code"]["minLength"], 2);
    assert_eq!(schema["properties"]["code"]["maxLength"], 10);
}

// ============================================================
// AC 2.3: json_schema includes doc comment descriptions
// ============================================================

#[test]
fn test_json_schema_descriptions_from_doc_comments() {
    let schema = AddParams::json_schema();
    assert_eq!(
        schema["properties"]["title"]["description"],
        "Task title"
    );
    assert_eq!(
        schema["properties"]["priority"]["description"],
        "Task priority (1-5)"
    );
    assert_eq!(
        schema["properties"]["tags"]["description"],
        "Tags for categorization"
    );
}

// ============================================================
// AC 2.3: from_value with all fields present
// ============================================================

#[test]
fn test_from_value_all_fields_present() {
    let args = json!({
        "title": "Buy groceries",
        "priority": 2,
        "tags": ["shopping"],
        "description": "From the store",
        "urgent": true
    });
    let params = AddParams::from_value(&args).unwrap();
    assert_eq!(params.title, "Buy groceries");
    assert_eq!(params.priority, Some(2));
    assert_eq!(params.tags, vec!["shopping"]);
    assert_eq!(params.description, Some("From the store".to_string()));
    assert_eq!(params.urgent, Some(true));
}

// ============================================================
// AC 2.3: from_value with missing required field -> error
// ============================================================

#[test]
fn test_from_value_missing_required_field_errors() {
    let args = json!({ "priority": 2 });
    let result = AddParams::from_value(&args);
    assert!(result.is_err());
}

#[test]
fn test_from_value_null_required_field_errors() {
    let args = json!({ "title": null });
    let result = AddParams::from_value(&args);
    assert!(result.is_err());
}

// ============================================================
// AC 2.3: from_value with empty optional Vec
// ============================================================

#[test]
fn test_from_value_missing_vec_defaults_to_empty() {
    let args = json!({ "title": "test" });
    let params = AddParams::from_value(&args).unwrap();
    assert!(params.tags.is_empty());
}

#[test]
fn test_from_value_explicit_empty_array() {
    let args = json!({ "title": "test", "tags": [] });
    let params = AddParams::from_value(&args).unwrap();
    assert!(params.tags.is_empty());
}

// ============================================================
// AC 2.3: from_value with missing optional fields
// ============================================================

#[test]
fn test_from_value_missing_optional_fields_are_none() {
    let args = json!({ "title": "test" });
    let params = AddParams::from_value(&args).unwrap();
    assert_eq!(params.priority, None);
    assert_eq!(params.description, None);
    assert_eq!(params.urgent, None);
}

// ============================================================
// AC 2.3: empty params struct
// ============================================================

#[test]
fn test_empty_params_schema() {
    let schema = EmptyParams::json_schema();
    assert_eq!(schema["type"], "object");
}

#[test]
fn test_empty_params_from_value() {
    let result = EmptyParams::from_value(&json!({}));
    assert!(result.is_ok());
}
