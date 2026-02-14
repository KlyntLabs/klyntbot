//! ParamExtractor — zero-cost helper for extracting typed values from `serde_json::Value` args.
//!
//! Eliminates the repetitive `args.get("x").and_then(|v| v.as_str()).ok_or_else(...)` pattern
//! used across all tool implementations. Provides clear, consistent error messages that
//! include the parameter name.
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::params::ParamExtractor;
//!
//! async fn execute(&self, args: Value, _ctx: &RoutingContext) -> Result<String> {
//!     let p = ParamExtractor::new(&args);
//!     let path = p.required_str("path")?;
//!     let count = p.i64_or("count", 10)?;
//!     // ...
//! }
//! ```

use common::{KlyntbotError, ToolError};
use serde_json::Value;

/// Zero-cost wrapper around `&Value` for ergonomic parameter extraction.
///
/// - `required_*` → `Err` if absent OR wrong type
/// - `optional_*` → `Ok(None)` if absent, `Err` if present but wrong type
/// - `str_or` / `i64_or` → default if absent, `Err` if present but wrong type
pub struct ParamExtractor<'a> {
    args: &'a Value,
}

// ── Private error helpers ────────────────────────────────────────────

fn missing_param(name: &str) -> KlyntbotError {
    ToolError::InvalidParams(format!("missing required '{}' parameter", name)).into()
}

fn wrong_type(name: &str, expected: &str) -> KlyntbotError {
    ToolError::InvalidParams(format!("'{}' must be {}", name, expected)).into()
}

impl<'a> ParamExtractor<'a> {
    /// Wrap a `&Value` (the `args` object passed to `Tool::execute`).
    pub fn new(args: &'a Value) -> Self {
        Self { args }
    }

    // ── Required extractors ──────────────────────────────────────────

    /// Extract a required string parameter.
    pub fn required_str(&self, name: &str) -> Result<&'a str, KlyntbotError> {
        match self.args.get(name) {
            None | Some(Value::Null) => Err(missing_param(name)),
            Some(v) => v.as_str().ok_or_else(|| wrong_type(name, "a string")),
        }
    }

    /// Extract a required i64 parameter.
    pub fn required_i64(&self, name: &str) -> Result<i64, KlyntbotError> {
        match self.args.get(name) {
            None | Some(Value::Null) => Err(missing_param(name)),
            Some(v) => v.as_i64().ok_or_else(|| wrong_type(name, "an integer")),
        }
    }

    /// Extract a required u64 parameter.
    pub fn required_u64(&self, name: &str) -> Result<u64, KlyntbotError> {
        match self.args.get(name) {
            None | Some(Value::Null) => Err(missing_param(name)),
            Some(v) => v
                .as_u64()
                .ok_or_else(|| wrong_type(name, "a positive integer")),
        }
    }

    /// Extract a required boolean parameter.
    pub fn required_bool(&self, name: &str) -> Result<bool, KlyntbotError> {
        match self.args.get(name) {
            None | Some(Value::Null) => Err(missing_param(name)),
            Some(v) => v.as_bool().ok_or_else(|| wrong_type(name, "a boolean")),
        }
    }

    /// Extract a required array parameter.
    pub fn required_array(&self, name: &str) -> Result<&'a Vec<Value>, KlyntbotError> {
        match self.args.get(name) {
            None | Some(Value::Null) => Err(missing_param(name)),
            Some(v) => v.as_array().ok_or_else(|| wrong_type(name, "an array")),
        }
    }

    /// Extract a required object parameter.
    pub fn required_object(
        &self,
        name: &str,
    ) -> Result<&'a serde_json::Map<String, Value>, KlyntbotError> {
        match self.args.get(name) {
            None | Some(Value::Null) => Err(missing_param(name)),
            Some(v) => v.as_object().ok_or_else(|| wrong_type(name, "an object")),
        }
    }

    // ── Optional extractors ──────────────────────────────────────────
    // Ok(None) if absent, Err if present but wrong type.

    /// Extract an optional string parameter.
    pub fn optional_str(&self, name: &str) -> Result<Option<&'a str>, KlyntbotError> {
        match self.args.get(name) {
            None | Some(Value::Null) => Ok(None),
            Some(v) => v
                .as_str()
                .map(Some)
                .ok_or_else(|| wrong_type(name, "a string")),
        }
    }

    /// Extract an optional string, returning `default` if absent.
    /// Returns `Err` if the key is present but not a string.
    pub fn str_or(&self, name: &str, default: &'a str) -> Result<&'a str, KlyntbotError> {
        match self.args.get(name) {
            None | Some(Value::Null) => Ok(default),
            Some(v) => v.as_str().ok_or_else(|| wrong_type(name, "a string")),
        }
    }

    /// Extract an optional i64 parameter.
    pub fn optional_i64(&self, name: &str) -> Result<Option<i64>, KlyntbotError> {
        match self.args.get(name) {
            None | Some(Value::Null) => Ok(None),
            Some(v) => v
                .as_i64()
                .map(Some)
                .ok_or_else(|| wrong_type(name, "an integer")),
        }
    }

    /// Extract an optional i64, returning `default` if absent.
    /// Returns `Err` if the key is present but not an integer.
    pub fn i64_or(&self, name: &str, default: i64) -> Result<i64, KlyntbotError> {
        match self.args.get(name) {
            None | Some(Value::Null) => Ok(default),
            Some(v) => v.as_i64().ok_or_else(|| wrong_type(name, "an integer")),
        }
    }

    /// Extract an optional u64 parameter.
    pub fn optional_u64(&self, name: &str) -> Result<Option<u64>, KlyntbotError> {
        match self.args.get(name) {
            None | Some(Value::Null) => Ok(None),
            Some(v) => v
                .as_u64()
                .map(Some)
                .ok_or_else(|| wrong_type(name, "a positive integer")),
        }
    }

    /// Extract an optional boolean parameter.
    pub fn optional_bool(&self, name: &str) -> Result<Option<bool>, KlyntbotError> {
        match self.args.get(name) {
            None | Some(Value::Null) => Ok(None),
            Some(v) => v
                .as_bool()
                .map(Some)
                .ok_or_else(|| wrong_type(name, "a boolean")),
        }
    }

    /// Extract an optional array parameter.
    pub fn optional_array(&self, name: &str) -> Result<Option<&'a Vec<Value>>, KlyntbotError> {
        match self.args.get(name) {
            None | Some(Value::Null) => Ok(None),
            Some(v) => v
                .as_array()
                .map(Some)
                .ok_or_else(|| wrong_type(name, "an array")),
        }
    }

    /// Extract string values from an optional JSON array of strings.
    /// Returns empty Vec if key absent, silently filters non-string elements.
    pub fn string_array_or_empty(&self, name: &str) -> Result<Vec<String>, KlyntbotError> {
        match self.args.get(name) {
            None | Some(Value::Null) => Ok(Vec::new()),
            Some(v) => {
                let arr = v.as_array().ok_or_else(|| wrong_type(name, "an array"))?;
                Ok(arr
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── required_str ─────────────────────────────────────────────────

    #[test]
    fn required_str_present() {
        let args = json!({"path": "/tmp/test.txt"});
        let p = ParamExtractor::new(&args);
        assert_eq!(p.required_str("path").unwrap(), "/tmp/test.txt");
    }

    #[test]
    fn required_str_missing() {
        let args = json!({});
        let p = ParamExtractor::new(&args);
        let err = p.required_str("path").unwrap_err();
        assert!(err
            .to_string()
            .contains("missing required 'path' parameter"));
    }

    #[test]
    fn required_str_wrong_type() {
        let args = json!({"path": 123});
        let p = ParamExtractor::new(&args);
        let err = p.required_str("path").unwrap_err();
        assert!(err.to_string().contains("'path' must be a string"));
    }

    #[test]
    fn required_str_null_treated_as_missing() {
        let args = json!({"path": null});
        let p = ParamExtractor::new(&args);
        let err = p.required_str("path").unwrap_err();
        assert!(err.to_string().contains("missing required 'path'"));
    }

    // ── required_i64 ─────────────────────────────────────────────────

    #[test]
    fn required_i64_present() {
        let args = json!({"count": 42});
        let p = ParamExtractor::new(&args);
        assert_eq!(p.required_i64("count").unwrap(), 42);
    }

    #[test]
    fn required_i64_missing() {
        let args = json!({});
        let p = ParamExtractor::new(&args);
        let err = p.required_i64("count").unwrap_err();
        assert!(err.to_string().contains("missing required 'count'"));
    }

    #[test]
    fn required_i64_wrong_type() {
        let args = json!({"count": "five"});
        let p = ParamExtractor::new(&args);
        let err = p.required_i64("count").unwrap_err();
        assert!(err.to_string().contains("'count' must be an integer"));
    }

    // ── required_u64 ─────────────────────────────────────────────────

    #[test]
    fn required_u64_present() {
        let args = json!({"seconds": 60});
        let p = ParamExtractor::new(&args);
        assert_eq!(p.required_u64("seconds").unwrap(), 60);
    }

    #[test]
    fn required_u64_missing() {
        let args = json!({});
        let p = ParamExtractor::new(&args);
        assert!(p.required_u64("seconds").is_err());
    }

    #[test]
    fn required_u64_negative() {
        let args = json!({"seconds": -1});
        let p = ParamExtractor::new(&args);
        let err = p.required_u64("seconds").unwrap_err();
        assert!(err
            .to_string()
            .contains("'seconds' must be a positive integer"));
    }

    // ── required_bool ────────────────────────────────────────────────

    #[test]
    fn required_bool_present() {
        let args = json!({"enabled": true});
        let p = ParamExtractor::new(&args);
        assert!(p.required_bool("enabled").unwrap());
    }

    #[test]
    fn required_bool_wrong_type() {
        let args = json!({"enabled": "yes"});
        let p = ParamExtractor::new(&args);
        let err = p.required_bool("enabled").unwrap_err();
        assert!(err.to_string().contains("'enabled' must be a boolean"));
    }

    // ── required_array ───────────────────────────────────────────────

    #[test]
    fn required_array_present() {
        let args = json!({"tags": ["a", "b"]});
        let p = ParamExtractor::new(&args);
        assert_eq!(p.required_array("tags").unwrap().len(), 2);
    }

    #[test]
    fn required_array_wrong_type() {
        let args = json!({"tags": "not-an-array"});
        let p = ParamExtractor::new(&args);
        let err = p.required_array("tags").unwrap_err();
        assert!(err.to_string().contains("'tags' must be an array"));
    }

    // ── required_object ──────────────────────────────────────────────

    #[test]
    fn required_object_present() {
        let args = json!({"config": {"key": "value"}});
        let p = ParamExtractor::new(&args);
        let obj = p.required_object("config").unwrap();
        assert_eq!(obj.get("key").unwrap().as_str().unwrap(), "value");
    }

    #[test]
    fn required_object_wrong_type() {
        let args = json!({"config": "not-an-object"});
        let p = ParamExtractor::new(&args);
        let err = p.required_object("config").unwrap_err();
        assert!(err.to_string().contains("'config' must be an object"));
    }

    // ── optional_str ─────────────────────────────────────────────────

    #[test]
    fn optional_str_present() {
        let args = json!({"label": "test"});
        let p = ParamExtractor::new(&args);
        assert_eq!(p.optional_str("label").unwrap(), Some("test"));
    }

    #[test]
    fn optional_str_absent() {
        let args = json!({});
        let p = ParamExtractor::new(&args);
        assert_eq!(p.optional_str("label").unwrap(), None);
    }

    #[test]
    fn optional_str_wrong_type() {
        let args = json!({"label": 123});
        let p = ParamExtractor::new(&args);
        let err = p.optional_str("label").unwrap_err();
        assert!(err.to_string().contains("'label' must be a string"));
    }

    #[test]
    fn optional_str_null_is_none() {
        let args = json!({"label": null});
        let p = ParamExtractor::new(&args);
        assert_eq!(p.optional_str("label").unwrap(), None);
    }

    // ── str_or ───────────────────────────────────────────────────────

    #[test]
    fn str_or_present() {
        let args = json!({"name": "custom"});
        let p = ParamExtractor::new(&args);
        assert_eq!(p.str_or("name", "default").unwrap(), "custom");
    }

    #[test]
    fn str_or_absent_returns_default() {
        let args = json!({});
        let p = ParamExtractor::new(&args);
        assert_eq!(p.str_or("name", "default").unwrap(), "default");
    }

    #[test]
    fn str_or_wrong_type_errors() {
        let args = json!({"name": 42});
        let p = ParamExtractor::new(&args);
        let err = p.str_or("name", "default").unwrap_err();
        assert!(err.to_string().contains("'name' must be a string"));
    }

    // ── optional_i64 / i64_or ────────────────────────────────────────

    #[test]
    fn optional_i64_present() {
        let args = json!({"count": 5});
        let p = ParamExtractor::new(&args);
        assert_eq!(p.optional_i64("count").unwrap(), Some(5));
    }

    #[test]
    fn optional_i64_absent() {
        let args = json!({});
        let p = ParamExtractor::new(&args);
        assert_eq!(p.optional_i64("count").unwrap(), None);
    }

    #[test]
    fn optional_i64_wrong_type() {
        let args = json!({"count": "five"});
        let p = ParamExtractor::new(&args);
        assert!(p.optional_i64("count").is_err());
    }

    #[test]
    fn i64_or_absent_returns_default() {
        let args = json!({});
        let p = ParamExtractor::new(&args);
        assert_eq!(p.i64_or("count", 10).unwrap(), 10);
    }

    #[test]
    fn i64_or_present() {
        let args = json!({"count": 42});
        let p = ParamExtractor::new(&args);
        assert_eq!(p.i64_or("count", 10).unwrap(), 42);
    }

    #[test]
    fn i64_or_wrong_type_errors() {
        let args = json!({"count": "nope"});
        let p = ParamExtractor::new(&args);
        assert!(p.i64_or("count", 10).is_err());
    }

    // ── optional_u64 ─────────────────────────────────────────────────

    #[test]
    fn optional_u64_present() {
        let args = json!({"seconds": 60});
        let p = ParamExtractor::new(&args);
        assert_eq!(p.optional_u64("seconds").unwrap(), Some(60));
    }

    #[test]
    fn optional_u64_absent() {
        let args = json!({});
        let p = ParamExtractor::new(&args);
        assert_eq!(p.optional_u64("seconds").unwrap(), None);
    }

    #[test]
    fn optional_u64_wrong_type() {
        let args = json!({"seconds": "sixty"});
        let p = ParamExtractor::new(&args);
        assert!(p.optional_u64("seconds").is_err());
    }

    // ── optional_bool ────────────────────────────────────────────────

    #[test]
    fn optional_bool_absent() {
        let args = json!({});
        let p = ParamExtractor::new(&args);
        assert_eq!(p.optional_bool("flag").unwrap(), None);
    }

    #[test]
    fn optional_bool_present() {
        let args = json!({"flag": false});
        let p = ParamExtractor::new(&args);
        assert_eq!(p.optional_bool("flag").unwrap(), Some(false));
    }

    #[test]
    fn optional_bool_wrong_type() {
        let args = json!({"flag": "true"});
        let p = ParamExtractor::new(&args);
        assert!(p.optional_bool("flag").is_err());
    }

    // ── optional_array ───────────────────────────────────────────────

    #[test]
    fn optional_array_present() {
        let args = json!({"items": [1, 2, 3]});
        let p = ParamExtractor::new(&args);
        assert_eq!(p.optional_array("items").unwrap().unwrap().len(), 3);
    }

    #[test]
    fn optional_array_absent() {
        let args = json!({});
        let p = ParamExtractor::new(&args);
        assert_eq!(p.optional_array("items").unwrap(), None);
    }

    #[test]
    fn optional_array_wrong_type() {
        let args = json!({"items": "not-array"});
        let p = ParamExtractor::new(&args);
        assert!(p.optional_array("items").is_err());
    }

    // ── string_array_or_empty ────────────────────────────────────────

    #[test]
    fn string_array_or_empty_present() {
        let args = json!({"tags": ["rust", "async"]});
        let p = ParamExtractor::new(&args);
        assert_eq!(
            p.string_array_or_empty("tags").unwrap(),
            vec!["rust", "async"]
        );
    }

    #[test]
    fn string_array_or_empty_absent() {
        let args = json!({});
        let p = ParamExtractor::new(&args);
        assert!(p.string_array_or_empty("tags").unwrap().is_empty());
    }

    #[test]
    fn string_array_or_empty_filters_non_strings() {
        let args = json!({"tags": ["valid", 123, "also_valid"]});
        let p = ParamExtractor::new(&args);
        assert_eq!(
            p.string_array_or_empty("tags").unwrap(),
            vec!["valid", "also_valid"]
        );
    }

    #[test]
    fn string_array_or_empty_wrong_type() {
        let args = json!({"tags": "not-an-array"});
        let p = ParamExtractor::new(&args);
        let err = p.string_array_or_empty("tags").unwrap_err();
        assert!(err.to_string().contains("'tags' must be an array"));
    }
}
