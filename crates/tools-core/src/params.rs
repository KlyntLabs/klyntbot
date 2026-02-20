//! ParamExtractor — zero-cost helper for extracting typed values from `serde_json::Value` args.
//!
//! Eliminates the repetitive `args.get("x").and_then(|v| v.as_str()).ok_or_else(...)` pattern
//! used across all tool implementations. Provides clear, consistent error messages that
//! include the parameter name.

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

        assert_eq!(p.required_str("path").unwrap(), "/tmp/test.txt");
        assert_eq!(p.required_i64("count").unwrap(), 42);
        assert_eq!(p.required_u64("seconds").unwrap(), 60);
        assert!(p.required_bool("enabled").unwrap());
        assert_eq!(p.required_array("tags").unwrap().len(), 2);
        assert_eq!(
            p.required_object("config")
                .unwrap()
                .get("key")
                .unwrap()
                .as_str()
                .unwrap(),
            "value"
        );
    }

    #[test]
    fn test_required_extractors_missing() {
        let args = json!({});
        let p = ParamExtractor::new(&args);

        assert!(p.required_str("x").is_err());
        assert!(p.required_i64("x").is_err());
        assert!(p.required_u64("x").is_err());
        assert!(p.required_bool("x").is_err());
        assert!(p.required_array("x").is_err());
        assert!(p.required_object("x").is_err());
    }

    #[test]
    fn test_optional_extractors() {
        let args = json!({"s": "hello"});
        let p = ParamExtractor::new(&args);
        assert_eq!(p.optional_str("s").unwrap(), Some("hello"));
        assert_eq!(p.optional_str("missing").unwrap(), None);
    }
}
