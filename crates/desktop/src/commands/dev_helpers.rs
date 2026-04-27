//! Shared helpers for dev server command dispatch.
//!
//! Only compiled in debug builds (`#[cfg(debug_assertions)]`).
//! Each command module uses these to serialize results and extract
//! JSON parameters in its `dispatch_dev` function.

use desktop_shared::{errors::ApiError, CommandResult};
use serde_json::Value;

/// Serialize a `CommandResult<T>` into `CommandResult<Value>`.
pub fn val<T: serde::Serialize>(result: CommandResult<T>) -> CommandResult<Value> {
    result.map(|v| serde_json::to_value(v).unwrap_or(Value::Null))
}

/// Serialize a handler result (value + entity updates), discarding the updates.
/// In dev mode there is no Tauri event bus, so entity updates are dropped.
pub fn val_rh<T: serde::Serialize, U>(result: CommandResult<(T, U)>) -> CommandResult<Value> {
    result.map(|(v, _)| serde_json::to_value(v).unwrap_or(Value::Null))
}

/// Extract an optional typed field from the JSON body.
pub fn get<T: serde::de::DeserializeOwned>(body: &Value, key: &str) -> Option<T> {
    body.get(key)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
}

/// Extract a required string field.
pub fn get_str(body: &Value, key: &str) -> CommandResult<String> {
    body.get(key)
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| ApiError::new("VALIDATION", format!("missing required field: {key}")))
}

/// Deserialize structured params from the body (tries `body.params` first, then `body` itself).
pub fn parse_params<T: serde::de::DeserializeOwned>(body: &Value) -> CommandResult<T> {
    serde_json::from_value(body.get("params").cloned().unwrap_or(body.clone()))
        .map_err(|e| ApiError::new("VALIDATION", e.to_string()))
}

/// Require a typed field (returns validation error if missing).
pub fn require<T: serde::de::DeserializeOwned>(body: &Value, key: &str) -> CommandResult<T> {
    get(body, key)
        .ok_or_else(|| ApiError::new("VALIDATION", format!("missing required field: {key}")))
}

/// Early-return with `Some(Err(e))` from a `dispatch_dev` function.
macro_rules! try_field {
    ($expr:expr) => {
        match $expr {
            Ok(v) => v,
            Err(e) => return Some(Err(e)),
        }
    };
}
pub(crate) use try_field;
