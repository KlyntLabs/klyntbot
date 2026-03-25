//! JSON Schema validation for tool parameters.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use serde_json::Value;

/// Global cache for compiled regexes used in JSON Schema `pattern` validation.
/// Tool schemas define a small, fixed set of patterns that are reused on every
/// invocation — caching avoids recompiling on each call.
fn regex_cache() -> &'static Mutex<HashMap<String, regex::Regex>> {
    static CACHE: OnceLock<Mutex<HashMap<String, regex::Regex>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Get or compile a regex pattern, caching the result.
fn get_or_compile_regex(pattern: &str) -> Option<regex::Regex> {
    let cache = regex_cache();
    let mut map = cache.lock().unwrap();
    if let Some(re) = map.get(pattern) {
        return Some(re.clone());
    }
    match regex::Regex::new(pattern) {
        Ok(re) => {
            map.insert(pattern.to_string(), re.clone());
            Some(re)
        }
        Err(_) => None,
    }
}

/// Validate a value against a JSON Schema, returning human-readable error messages.
pub(crate) fn validate_value(val: &Value, schema: &Value, path: &str) -> Vec<String> {
    let mut errors = Vec::new();
    let label = if path.is_empty() {
        "parameter".to_string()
    } else {
        path.to_string()
    };

    let schema_type = schema.get("type").and_then(|t| t.as_str());

    // Type validation
    match schema_type {
        Some("string") => {
            if !val.is_string() {
                errors.push(format!("{} should be string", label));
                return errors;
            }
            let s = val.as_str().unwrap();
            if let Some(min_len) = schema.get("minLength").and_then(|v| v.as_u64()) {
                if s.len() < min_len as usize {
                    errors.push(format!("{} must be at least {} chars", label, min_len));
                }
            }
            if let Some(max_len) = schema.get("maxLength").and_then(|v| v.as_u64()) {
                if s.len() > max_len as usize {
                    errors.push(format!("{} must be at most {} chars", label, max_len));
                }
            }
            if let Some(pattern) = schema.get("pattern").and_then(|v| v.as_str()) {
                if let Some(re) = get_or_compile_regex(pattern) {
                    if !re.is_match(s) {
                        errors.push(format!("{} must match pattern {}", label, pattern));
                    }
                }
            }
        }
        Some("integer") => {
            if !val.is_i64() && !val.is_u64() {
                errors.push(format!("{} should be integer", label));
                return errors;
            }
            let n = val.as_i64().unwrap_or(0);
            if let Some(min) = schema.get("minimum").and_then(|v| v.as_i64()) {
                if n < min {
                    errors.push(format!("{} must be >= {}", label, min));
                }
            }
            if let Some(max) = schema.get("maximum").and_then(|v| v.as_i64()) {
                if n > max {
                    errors.push(format!("{} must be <= {}", label, max));
                }
            }
        }
        Some("number") => {
            if !val.is_f64() && !val.is_i64() && !val.is_u64() {
                errors.push(format!("{} should be number", label));
                return errors;
            }
            let n = val.as_f64().unwrap_or(0.0);
            if let Some(min) = schema.get("minimum").and_then(|v| v.as_f64()) {
                if n < min {
                    errors.push(format!("{} must be >= {}", label, min));
                }
            }
            if let Some(max) = schema.get("maximum").and_then(|v| v.as_f64()) {
                if n > max {
                    errors.push(format!("{} must be <= {}", label, max));
                }
            }
        }
        Some("boolean") => {
            if !val.is_boolean() {
                errors.push(format!("{} should be boolean", label));
                return errors;
            }
        }
        Some("array") => {
            if !val.is_array() {
                errors.push(format!("{} should be array", label));
                return errors;
            }
            if let Some(arr) = val.as_array() {
                if let Some(min) = schema.get("minItems").and_then(|v| v.as_u64()) {
                    if arr.len() < min as usize {
                        errors.push(format!("{} must have at least {} items", label, min));
                    }
                }
                if let Some(max) = schema.get("maxItems").and_then(|v| v.as_u64()) {
                    if arr.len() > max as usize {
                        errors.push(format!("{} must have at most {} items", label, max));
                    }
                }
                if let Some(items_schema) = schema.get("items") {
                    for (i, item) in arr.iter().enumerate() {
                        let item_path = format!("{}[{}]", path, i);
                        errors.extend(validate_value(item, items_schema, &item_path));
                    }
                }
            }
        }
        Some("object") => {
            if !val.is_object() {
                errors.push(format!("{} should be object", label));
                return errors;
            }
            let obj = val.as_object().unwrap();

            // Check required fields
            if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
                for req in required {
                    if let Some(field_name) = req.as_str() {
                        if !obj.contains_key(field_name) {
                            let field_path = if path.is_empty() {
                                field_name.to_string()
                            } else {
                                format!("{}.{}", path, field_name)
                            };
                            errors.push(format!("missing required {}", field_path));
                        }
                    }
                }
            }

            // Validate properties
            if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
                for (key, value) in obj.iter() {
                    if let Some(prop_schema) = properties.get(key) {
                        let prop_path = if path.is_empty() {
                            key.clone()
                        } else {
                            format!("{}.{}", path, key)
                        };
                        errors.extend(validate_value(value, prop_schema, &prop_path));
                    }
                }
            }

            // Check additionalProperties: false
            if let Some(false) = schema.get("additionalProperties").and_then(|v| v.as_bool()) {
                if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
                    for key in obj.keys() {
                        if !properties.contains_key(key) {
                            let prop_path = if path.is_empty() {
                                key.clone()
                            } else {
                                format!("{}.{}", path, key)
                            };
                            errors.push(format!("unexpected property {}", prop_path));
                        }
                    }
                }
            }
        }
        _ => {}
    }

    // Enum validation
    if let Some(enum_values) = schema.get("enum").and_then(|e| e.as_array()) {
        if !enum_values.contains(val) {
            errors.push(format!("{} must be one of {:?}", label, enum_values));
        }
    }

    // oneOf: must match exactly one subschema
    if let Some(schemas) = schema.get("oneOf").and_then(|s| s.as_array()) {
        let match_count = schemas
            .iter()
            .filter(|s| validate_value(val, s, path).is_empty())
            .count();
        if match_count != 1 {
            errors.push(format!(
                "{} must match exactly one of oneOf schemas (matched {})",
                label, match_count
            ));
        }
    }

    // anyOf: must match at least one subschema
    if let Some(schemas) = schema.get("anyOf").and_then(|s| s.as_array()) {
        let matches_any = schemas
            .iter()
            .any(|s| validate_value(val, s, path).is_empty());
        if !matches_any {
            errors.push(format!(
                "{} must match at least one of anyOf schemas",
                label
            ));
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_validate_one_of_matches_exactly_one() {
        let schema = json!({
            "oneOf": [
                { "type": "string" },
                { "type": "integer" }
            ]
        });
        assert!(validate_value(&json!("hello"), &schema, "").is_empty());
        assert!(validate_value(&json!(42), &schema, "").is_empty());
        assert!(!validate_value(&json!(true), &schema, "").is_empty());
    }

    #[test]
    fn test_validate_any_of_matches_at_least_one() {
        let schema = json!({
            "anyOf": [
                { "type": "string" },
                { "type": "integer" }
            ]
        });
        assert!(validate_value(&json!("hello"), &schema, "").is_empty());
        assert!(validate_value(&json!(42), &schema, "").is_empty());
        assert!(!validate_value(&json!(true), &schema, "").is_empty());
    }

    #[test]
    fn test_validate_pattern() {
        let schema = json!({
            "type": "string",
            "pattern": "^[a-z]+$"
        });
        assert!(validate_value(&json!("hello"), &schema, "").is_empty());
        assert!(!validate_value(&json!("Hello123"), &schema, "").is_empty());
    }

    #[test]
    fn test_validate_additional_properties_false() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" }
            },
            "additionalProperties": false
        });
        assert!(validate_value(&json!({"name": "test"}), &schema, "").is_empty());
        assert!(!validate_value(&json!({"name": "test", "extra": true}), &schema, "").is_empty());
    }

    #[test]
    fn test_validate_min_max_items() {
        let schema = json!({
            "type": "array",
            "items": { "type": "integer" },
            "minItems": 2,
            "maxItems": 4
        });
        assert!(!validate_value(&json!([1]), &schema, "").is_empty());
        assert!(validate_value(&json!([1, 2]), &schema, "").is_empty());
        assert!(validate_value(&json!([1, 2, 3, 4]), &schema, "").is_empty());
        assert!(!validate_value(&json!([1, 2, 3, 4, 5]), &schema, "").is_empty());
    }

    #[test]
    fn test_validate_number_minimum_maximum() {
        let schema = json!({
            "type": "number",
            "minimum": 0.0,
            "maximum": 1.0
        });
        assert!(validate_value(&json!(0.5), &schema, "").is_empty());
        assert!(!validate_value(&json!(-0.1), &schema, "").is_empty());
        assert!(!validate_value(&json!(1.5), &schema, "").is_empty());
    }
}
