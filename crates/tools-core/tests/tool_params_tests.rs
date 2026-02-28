//! Tests for `#[derive(ToolParams)]` trait-based parameter derive.

use serde_json::json;
use tools_core::{ToolParams as ToolParamsTrait, ToolParams};

#[derive(Debug, ToolParams)]
pub struct SearchParams {
    /// Search query
    #[param(required)]
    pub query: String,

    /// Maximum results to return
    #[param(min = 1, max = 100)]
    pub limit: Option<u32>,

    /// Include archived items
    pub include_archived: Option<bool>,

    /// Tags to filter by
    pub tags: Vec<String>,

    /// Optional description
    pub description: Option<String>,
}

#[derive(Debug, ToolParams)]
pub struct EmptyParams;

// ── json_schema ─────────────────────────────────────────────

#[test]
fn test_schema_has_correct_types() {
    let schema = SearchParams::json_schema();
    assert_eq!(schema["properties"]["query"]["type"], "string");
    assert_eq!(schema["properties"]["limit"]["type"], "integer");
    assert_eq!(schema["properties"]["include_archived"]["type"], "boolean");
    assert_eq!(schema["properties"]["tags"]["type"], "array");
    assert_eq!(schema["properties"]["description"]["type"], "string");
}

#[test]
fn test_schema_required_fields() {
    let schema = SearchParams::json_schema();
    let required = schema["required"].as_array().unwrap();
    assert!(required.iter().any(|v| v == "query"));
    assert!(!required.iter().any(|v| v == "limit"));
}

#[test]
fn test_schema_descriptions_from_doc_comments() {
    let schema = SearchParams::json_schema();
    assert_eq!(schema["properties"]["query"]["description"], "Search query");
    assert_eq!(
        schema["properties"]["limit"]["description"],
        "Maximum results to return"
    );
}

#[test]
fn test_schema_constraints() {
    let schema = SearchParams::json_schema();
    assert_eq!(schema["properties"]["limit"]["minimum"], 1);
    assert_eq!(schema["properties"]["limit"]["maximum"], 100);
}

#[test]
fn test_empty_params_schema() {
    let schema = EmptyParams::json_schema();
    assert_eq!(schema["type"], "object");
}

// ── from_args ───────────────────────────────────────────────

#[test]
fn test_from_args_all_fields() {
    let args = json!({
        "query": "rust async",
        "limit": 10,
        "include_archived": true,
        "tags": ["programming", "rust"],
        "description": "Search for async patterns"
    });
    let params = SearchParams::from_args(args).unwrap();
    assert_eq!(params.query, "rust async");
    assert_eq!(params.limit, Some(10));
    assert_eq!(params.include_archived, Some(true));
    assert_eq!(params.tags, vec!["programming", "rust"]);
    assert_eq!(
        params.description,
        Some("Search for async patterns".to_string())
    );
}

#[test]
fn test_from_args_required_only() {
    let args = json!({ "query": "test" });
    let params = SearchParams::from_args(args).unwrap();
    assert_eq!(params.query, "test");
    assert_eq!(params.limit, None);
    assert_eq!(params.include_archived, None);
    assert!(params.tags.is_empty());
    assert_eq!(params.description, None);
}

#[test]
fn test_from_args_missing_required_errors() {
    let args = json!({ "limit": 10 });
    let result = SearchParams::from_args(args);
    assert!(result.is_err());
}

#[test]
fn test_from_args_empty_params() {
    let result = EmptyParams::from_args(json!({}));
    assert!(result.is_ok());
}

// ── Trait usage ─────────────────────────────────────────────

/// Verify the trait can be used generically.
fn parse_generic<P: ToolParamsTrait>(args: serde_json::Value) -> common::Result<P> {
    P::from_args(args)
}

#[test]
fn test_generic_trait_usage() {
    let params: SearchParams = parse_generic(json!({ "query": "hello" })).unwrap();
    assert_eq!(params.query, "hello");
}
