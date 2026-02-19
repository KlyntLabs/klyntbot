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

    /// Extract an optional f64 parameter.
    pub fn optional_f64(&self, name: &str) -> Result<Option<f64>, KlyntbotError> {
        match self.args.get(name) {
            None | Some(Value::Null) => Ok(None),
            Some(v) => v
                .as_f64()
                .map(Some)
                .ok_or_else(|| wrong_type(name, "a number")),
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

    #[test]
    fn test_required_extractors_present() {
        let args = json!({
            "path": "/tmp/test.txt",
            "count": 42,
            "seconds": 60,
            "enabled": true,
            "tags": ["a", "b"],
            "config": {"key": "value"}
        });
        let p = ParamExtractor::new(&args);

        assert_eq!(
            p.required_str("path").unwrap(),
            "/tmp/test.txt",
            "expected valid str extraction"
        );
        assert_eq!(
            p.required_i64("count").unwrap(),
            42,
            "expected valid i64 extraction"
        );
        assert_eq!(
            p.required_u64("seconds").unwrap(),
            60,
            "expected valid u64 extraction"
        );
        assert!(
            p.required_bool("enabled").unwrap(),
            "expected valid bool extraction"
        );
        assert_eq!(
            p.required_array("tags").unwrap().len(),
            2,
            "expected valid array extraction"
        );
        assert_eq!(
            p.required_object("config")
                .unwrap()
                .get("key")
                .unwrap()
                .as_str()
                .unwrap(),
            "value",
            "expected valid object extraction"
        );
    }

    #[test]
    fn test_required_extractors_missing() {
        let args = json!({});
        let p = ParamExtractor::new(&args);

        let labels = ["str", "i64", "u64", "bool", "array", "object"];
        let results: Vec<Result<(), KlyntbotError>> = vec![
            p.required_str("x").map(|_| ()),
            p.required_i64("x").map(|_| ()),
            p.required_u64("x").map(|_| ()),
            p.required_bool("x").map(|_| ()),
            p.required_array("x").map(|_| ()),
            p.required_object("x").map(|_| ()),
        ];

        for (label, result) in labels.iter().zip(results) {
            let err = result.unwrap_err();
            assert!(
                err.to_string().contains("missing required 'x'"),
                "expected MissingRequired error for required_{label}"
            );
        }
    }

    #[test]
    fn test_required_extractors_wrong_type() {
        let cases: Vec<(&str, Value, &str)> = vec![
            ("str", json!({"x": 123}), "'x' must be a string"),
            ("i64", json!({"x": "five"}), "'x' must be an integer"),
            (
                "u64",
                json!({"x": "sixty"}),
                "'x' must be a positive integer",
            ),
            ("bool", json!({"x": "yes"}), "'x' must be a boolean"),
            ("array", json!({"x": "not-array"}), "'x' must be an array"),
            (
                "object",
                json!({"x": "not-object"}),
                "'x' must be an object",
            ),
        ];

        for (label, args, expected_msg) in &cases {
            let p = ParamExtractor::new(args);
            let result: Result<(), KlyntbotError> = match *label {
                "str" => p.required_str("x").map(|_| ()),
                "i64" => p.required_i64("x").map(|_| ()),
                "u64" => p.required_u64("x").map(|_| ()),
                "bool" => p.required_bool("x").map(|_| ()),
                "array" => p.required_array("x").map(|_| ()),
                "object" => p.required_object("x").map(|_| ()),
                _ => unreachable!(),
            };
            let err = result.unwrap_err();
            assert!(
                err.to_string().contains(expected_msg),
                "expected TypeMismatch for required_{label}: got {err}"
            );
        }
    }

    #[test]
    fn test_required_edge_cases() {
        // null treated as missing for required_str
        let args = json!({"path": null});
        let p = ParamExtractor::new(&args);
        let err = p.required_str("path").unwrap_err();
        assert!(
            err.to_string().contains("missing required 'path'"),
            "expected null to be treated as missing for required_str"
        );

        // negative value rejected by required_u64
        let args = json!({"n": -1});
        let p = ParamExtractor::new(&args);
        let err = p.required_u64("n").unwrap_err();
        assert!(
            err.to_string().contains("'n' must be a positive integer"),
            "expected negative value to fail for required_u64"
        );
    }

    #[test]
    fn test_optional_extractors_present_and_absent() {
        let present = json!({
            "s": "hello",
            "i": 5,
            "u": 60,
            "b": false,
            "a": [1, 2, 3]
        });
        let p = ParamExtractor::new(&present);

        assert_eq!(p.optional_str("s").unwrap(), Some("hello"), "optional_str present");
        assert_eq!(p.optional_i64("i").unwrap(), Some(5), "optional_i64 present");
        assert_eq!(p.optional_u64("u").unwrap(), Some(60), "optional_u64 present");
        assert_eq!(p.optional_bool("b").unwrap(), Some(false), "optional_bool present");
        assert_eq!(
            p.optional_array("a").unwrap().unwrap().len(),
            3,
            "optional_array present"
        );

        // All absent → None
        let absent = json!({});
        let p = ParamExtractor::new(&absent);

        assert_eq!(p.optional_str("s").unwrap(), None, "optional_str absent");
        assert_eq!(p.optional_i64("i").unwrap(), None, "optional_i64 absent");
        assert_eq!(p.optional_u64("u").unwrap(), None, "optional_u64 absent");
        assert_eq!(p.optional_bool("b").unwrap(), None, "optional_bool absent");
        assert_eq!(p.optional_array("a").unwrap(), None, "optional_array absent");

        // null also returns None (test with optional_str)
        let null_val = json!({"s": null});
        let p = ParamExtractor::new(&null_val);
        assert_eq!(p.optional_str("s").unwrap(), None, "optional_str null is None");
    }

    #[test]
    fn test_optional_extractors_wrong_type() {
        let cases: Vec<(&str, Value, &str)> = vec![
            ("str", json!({"x": 123}), "'x' must be a string"),
            ("i64", json!({"x": "five"}), "'x' must be an integer"),
            (
                "u64",
                json!({"x": "sixty"}),
                "'x' must be a positive integer",
            ),
            ("bool", json!({"x": "true"}), "'x' must be a boolean"),
            ("array", json!({"x": "not-array"}), "'x' must be an array"),
        ];

        for (label, args, expected_msg) in &cases {
            let p = ParamExtractor::new(args);
            let result: Result<(), KlyntbotError> = match *label {
                "str" => p.optional_str("x").map(|_| ()),
                "i64" => p.optional_i64("x").map(|_| ()),
                "u64" => p.optional_u64("x").map(|_| ()),
                "bool" => p.optional_bool("x").map(|_| ()),
                "array" => p.optional_array("x").map(|_| ()),
                _ => unreachable!(),
            };
            let err = result.unwrap_err();
            assert!(
                err.to_string().contains(expected_msg),
                "expected TypeMismatch for optional_{label}: got {err}"
            );
        }
    }

    #[test]
    fn test_or_default_extractors() {
        // str_or: present returns value, absent returns default, wrong type errors
        let args = json!({"name": "custom"});
        let p = ParamExtractor::new(&args);
        assert_eq!(p.str_or("name", "default").unwrap(), "custom", "str_or present");

        let args = json!({});
        let p = ParamExtractor::new(&args);
        assert_eq!(p.str_or("name", "default").unwrap(), "default", "str_or absent");

        let args = json!({"name": 42});
        let p = ParamExtractor::new(&args);
        let err = p.str_or("name", "default").unwrap_err();
        assert!(
            err.to_string().contains("'name' must be a string"),
            "str_or wrong type"
        );

        // i64_or: present returns value, absent returns default, wrong type errors
        let args = json!({"count": 42});
        let p = ParamExtractor::new(&args);
        assert_eq!(p.i64_or("count", 10).unwrap(), 42, "i64_or present");

        let args = json!({});
        let p = ParamExtractor::new(&args);
        assert_eq!(p.i64_or("count", 10).unwrap(), 10, "i64_or absent");

        let args = json!({"count": "nope"});
        let p = ParamExtractor::new(&args);
        let err = p.i64_or("count", 10).unwrap_err();
        assert!(
            err.to_string().contains("'count' must be an integer"),
            "i64_or wrong type"
        );
    }

    #[test]
    fn test_string_array_or_empty() {
        // Present: returns string values
        let args = json!({"tags": ["rust", "async"]});
        let p = ParamExtractor::new(&args);
        assert_eq!(
            p.string_array_or_empty("tags").unwrap(),
            vec!["rust", "async"],
            "string_array_or_empty present"
        );

        // Absent: returns empty vec
        let args = json!({});
        let p = ParamExtractor::new(&args);
        assert!(
            p.string_array_or_empty("tags").unwrap().is_empty(),
            "string_array_or_empty absent"
        );

        // Filters non-strings
        let args = json!({"tags": ["valid", 123, "also_valid"]});
        let p = ParamExtractor::new(&args);
        assert_eq!(
            p.string_array_or_empty("tags").unwrap(),
            vec!["valid", "also_valid"],
            "string_array_or_empty filters non-strings"
        );

        // Wrong type: errors
        let args = json!({"tags": "not-an-array"});
        let p = ParamExtractor::new(&args);
        let err = p.string_array_or_empty("tags").unwrap_err();
        assert!(
            err.to_string().contains("'tags' must be an array"),
            "string_array_or_empty wrong type"
        );
    }
}
